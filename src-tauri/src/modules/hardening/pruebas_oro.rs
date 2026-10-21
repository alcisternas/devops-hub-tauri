// Pruebas de equivalencia con el motor de referencia (Python).
//
// testdata/oro.json lo genera la implementacion de Python de Container
// Hardening Studio sobre un escaneo real de node:26.12.0-alpine3.24: para
// nueve configuraciones guarda el analisis, los dos planes, las
// recomendaciones y el Dockerfile completo. Aqui se ejecuta el port de Rust
// sobre el mismo escaneo y se exige la misma salida, campo por campo.

use serde_json::{json, Value};

use super::componentes::detectar_componentes;
use super::config::{adaptar, config_desde_perfil, perfiles, ConfigHardening};
use super::dockerfile::{generar_node_alpine, EntradaDockerfile, ImagenOrigen};
use super::escaner::{normalizar_trivy, Escaneo};
use super::impacto::analizar;
use super::planificador::{config_efectiva, construir_plan, recomendar};

const FIXTURE: &str = include_str!("testdata/trivy_node26_alpine324.json");
const ORO: &str = include_str!("testdata/oro.json");

fn oro() -> Value {
    serde_json::from_str(ORO).unwrap()
}

fn escaneo() -> Escaneo {
    let raw: Value = serde_json::from_str(FIXTURE).unwrap();
    normalizar_trivy(&raw, &format!("node@{}", oro()["digest"].as_str().unwrap()))
}

// Primera diferencia entre dos JSON, con su ruta, para que un fallo sea legible.
fn diferencia(a: &Value, b: &Value, ruta: &str) -> Option<String> {
    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            for k in x.keys().chain(y.keys()) {
                match (x.get(k), y.get(k)) {
                    (Some(p), Some(q)) => {
                        if let Some(d) = diferencia(p, q, &format!("{}.{}", ruta, k)) {
                            return Some(d);
                        }
                    }
                    (Some(_), None) => return Some(format!("{}.{}: solo en Rust", ruta, k)),
                    (None, Some(_)) => return Some(format!("{}.{}: solo en Python", ruta, k)),
                    (None, None) => {}
                }
            }
            None
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                return Some(format!("{}: {} elementos en Rust, {} en Python", ruta, x.len(), y.len()));
            }
            x.iter().zip(y).enumerate().find_map(|(i, (p, q))| diferencia(p, q, &format!("{}[{}]", ruta, i)))
        }
        _ if a == b => None,
        _ => {
            let corto = |v: &Value| {
                let s = v.to_string();
                if s.chars().count() > 300 { format!("{}…", s.chars().take(300).collect::<String>()) } else { s }
            };
            Some(format!("{}:\n  Rust:   {}\n  Python: {}", ruta, corto(a), corto(b)))
        }
    }
}

fn exigir(rust: &impl serde::Serialize, python: &Value, ruta: &str) {
    let r = serde_json::to_value(rust).unwrap();
    if let Some(d) = diferencia(&r, python, ruta) {
        panic!("{}", d);
    }
}

fn config_de(caso: &Value) -> ConfigHardening {
    serde_json::from_value(caso["config"].clone()).expect("configuracion de referencia valida")
}

fn imagen() -> ImagenOrigen {
    let o = oro();
    let env = o["image"]["env"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
    ImagenOrigen {
        pinned_ref: o["image"]["pinned_ref"].as_str().unwrap().to_string(),
        env,
        os_family: o["image"]["os"]["family"].as_str().map(String::from),
        os_name: o["image"]["os"]["name"].as_str().map(String::from),
    }
}

#[test]
fn los_componentes_detectados_coinciden() {
    let o = oro();
    // "installable" es un campo propio del hub; Studio no lo genera.
    let mut propio = serde_json::to_value(detectar_componentes(&escaneo(), "node")).unwrap();
    for c in propio["components"].as_array_mut().unwrap() {
        assert!(c.as_object_mut().unwrap().remove("installable").is_some());
    }
    exigir(&propio, &o["detected"], "detected");
}

#[test]
fn los_perfiles_incluidos_coinciden() {
    let o = oro();
    let casos = o["casos"].as_object().unwrap();
    let propios = perfiles();
    assert_eq!(propios.len(), 6);
    for p in &propios {
        let caso = casos.get(&p.nombre).unwrap_or_else(|| panic!("perfil {} sin referencia", p.nombre));
        exigir(&p.config, &caso["config"], &format!("perfil {}", p.nombre));
        assert_eq!(config_desde_perfil(&p.nombre).unwrap(), p.config);
    }
}

#[test]
fn analisis_planes_y_recomendaciones_coinciden() {
    let o = oro();
    let scan = escaneo();
    let detectados = detectar_componentes(&scan, "node");
    for (nombre, caso) in o["casos"].as_object().unwrap() {
        let cfg = config_de(caso);
        cfg.validar().unwrap_or_else(|e| panic!("{}: {}", nombre, e));
        exigir(&cfg, &caso["config"], &format!("{}.config", nombre));
        exigir(&analizar(&cfg, &scan), &caso["analisis"], &format!("{}.analisis", nombre));

        let p1 = construir_plan(&cfg, &scan, 1, None);
        let p2 = construir_plan(&cfg, &scan, 2, Some(&p1));
        exigir(&p1, &caso["plan1"], &format!("{}.plan1", nombre));
        exigir(&p2, &caso["plan2"], &format!("{}.plan2", nombre));
        exigir(&config_efectiva(&cfg, &p2), &caso["efectiva2"], &format!("{}.efectiva2", nombre));
        exigir(&recomendar(&cfg, &scan), &caso["recomendaciones"], &format!("{}.recomendaciones", nombre));

        let (adaptada, descartados) = adaptar(&cfg, &detectados);
        exigir(&json!({"config": adaptada, "descartados": descartados}), &caso["adaptada"], &format!("{}.adaptada", nombre));
    }
}

#[test]
fn el_dockerfile_es_identico_al_de_python() {
    let o = oro();
    let scan = escaneo();
    let detectados = detectar_componentes(&scan, "node");
    let img = imagen();
    let builder = o["builder_ref"].as_str().unwrap();
    let mut con_error = 0;

    for (nombre, caso) in o["casos"].as_object().unwrap() {
        let cfg = config_de(caso);
        let p1 = construir_plan(&cfg, &scan, 1, None);
        let p2 = construir_plan(&cfg, &scan, 2, Some(&p1));
        for (etiqueta, plan) in [("plan1", &p1), ("plan2", &p2)] {
            let esperado = &caso["dockerfile"][etiqueta];
            let ruta = format!("{}.dockerfile.{}", nombre, etiqueta);
            let entrada = EntradaDockerfile {
                cfg: &cfg,
                plan,
                imagen: &img,
                detectados: &detectados,
                builder_ref: builder,
                network_tests: true,
                npm_registry: "https://registry.npmjs.org/",
                nonce: "0",
                // Misma firma que Studio, para que el texto y la huella coincidan.
                generador: "Container Hardening Studio",
            };
            let r = generar_node_alpine(&entrada);
            if let Some(err) = esperado.get("error") {
                con_error += 1;
                assert_eq!(r.err().map(|e| e.to_string()).as_deref(), err.as_str(), "{}", ruta);
                continue;
            }
            let doc = r.unwrap_or_else(|e| panic!("{}: {}", ruta, e));
            assert_eq!(doc.text, esperado["text"].as_str().unwrap(), "{}: texto del Dockerfile", ruta);
            exigir(&doc.explanations, &esperado["explanations"], &format!("{}.explicaciones", ruta));
            exigir(&doc.expected_tests, &esperado["expected_tests"], &format!("{}.pruebas", ruta));
            exigir(&doc.meta, &esperado["meta"], &format!("{}.meta", ruta));

            // Studio guarda patches.json completo y, del resto, solo la longitud en caracteres.
            let archivos: serde_json::Map<String, Value> = doc
                .files
                .iter()
                .map(|(k, v)| (k.clone(), if k.ends_with("patches.json") { json!(v) } else { json!(format!("len={}", v.chars().count())) }))
                .collect();
            exigir(&Value::Object(archivos), &esperado["files"], &format!("{}.archivos", ruta));
            assert_eq!(doc.sha256, esperado["sha256"].as_str().unwrap(), "{}: huella", ruta);
        }
    }
    // Las configuraciones con Corepack deben fallar igual que en Studio: la prueba tiene que ejercitar ambos caminos.
    assert!(con_error >= 2, "se esperaban errores de generacion de referencia");
}

// Requiere Trivy instalado y red: se ejecuta a mano con
// `cargo test --lib hardening::pruebas_oro::escaneo_real -- --ignored --nocapture`.
// Escanea de verdad la misma imagen del oro y compara con el escaneo guardado.
#[tokio::test]
#[ignore]
async fn escaneo_real_de_la_imagen_del_oro() {
    use super::escaner::{escanear, Escaner};
    let o = oro();
    let pinned = o["image"]["pinned_ref"].as_str().unwrap();
    let real = escanear(Escaner::Trivy, pinned, "linux/amd64").await.expect("trivy debe escanear");
    let guardado = escaneo();
    println!("trivy {:?}", real.scanner.version);
    println!("real:      {:?}", real.counts);
    println!("guardado:  {:?}", guardado.counts);
    println!("os real: {:?} / guardado: {:?}", real.os, guardado.os);
    assert!(!real.vulnerabilities.is_empty());
    assert_eq!(real.os.family, guardado.os.family);
    assert_eq!(real.image.env.get("NODE_VERSION"), guardado.image.env.get("NODE_VERSION"));
}
