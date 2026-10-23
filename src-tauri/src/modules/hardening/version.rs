// Comparacion de versiones para elegir la correccion mas cercana de un hallazgo.
//
// Cubre lo que aparece en los escaneos: versiones numericas (5.0.11), las de
// apk con sufijo de revision (3.5.1-r0, se ignora la revision, igual que en
// Studio) y prerelease simples (1.0.0-rc.1). Lo demas es "no comparable" y
// quien llama decide: nunca se adivina un orden.

use std::cmp::Ordering;

#[derive(Debug, Clone, Eq)]
pub struct Version {
    pub release: Vec<u64>,
    // None = version final. Some((rango, numero)): dev < a < b < rc.
    pre: Option<(u8, u64)>,
}

impl Version {
    pub fn mayor(&self) -> u64 {
        self.release.first().copied().unwrap_or(0)
    }
}

// Igualdad coherente con el orden: 1.2 y 1.2.0 son la misma version.
impl PartialEq for Version {
    fn eq(&self, otro: &Self) -> bool {
        self.cmp(otro) == Ordering::Equal
    }
}

impl Ord for Version {
    fn cmp(&self, otro: &Self) -> Ordering {
        let n = self.release.len().max(otro.release.len());
        for i in 0..n {
            let a = self.release.get(i).copied().unwrap_or(0);
            let b = otro.release.get(i).copied().unwrap_or(0);
            if a != b {
                return a.cmp(&b);
            }
        }
        match (&self.pre, &otro.pre) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, otro: &Self) -> Option<Ordering> {
        Some(self.cmp(otro))
    }
}

// Interpreta una version; None si no es una version reconocible.
pub fn parsear(texto: &str) -> Option<Version> {
    // Revision de apk: "1.2.3-r0" -> "1.2.3"
    let base = match texto.find("-r") {
        Some(i) if texto[i + 2..].chars().all(|c| c.is_ascii_digit()) => &texto[..i],
        _ => texto,
    };
    let base = base.strip_prefix('v').unwrap_or(base);
    if base.is_empty() {
        return None;
    }

    let fin_numerico = base
        .char_indices()
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map(|(i, _)| i)
        .unwrap_or(base.len());
    let (numeros, resto) = base.split_at(fin_numerico);
    let numeros = numeros.trim_end_matches('.');
    if numeros.is_empty() {
        return None;
    }
    let mut release: Vec<u64> = Vec::new();
    for parte in numeros.split('.') {
        release.push(parte.parse::<u64>().ok()?);
    }

    let resto = resto.trim_start_matches(|c| c == '-' || c == '.' || c == '_');
    if resto.is_empty() {
        return Some(Version { release, pre: None });
    }
    let letras: String = resto.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let rango = match letras.to_ascii_lowercase().as_str() {
        "dev" => 0,
        "a" | "alpha" => 1,
        "b" | "beta" => 2,
        "c" | "rc" | "pre" | "preview" => 3,
        _ => return None,
    };
    let digitos: String = resto[letras.len()..]
        .trim_start_matches(|c| c == '-' || c == '.' || c == '_')
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    let numero = if digitos.is_empty() { 0 } else { digitos.parse().ok()? };
    Some(Version { release, pre: Some((rango, numero)) })
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn v(s: &str) -> Version {
        parsear(s).unwrap_or_else(|| panic!("no parsea {}", s))
    }

    #[test]
    fn ordena_versiones_numericas() {
        assert!(v("5.0.11") > v("5.0.9"));
        assert!(v("6.28.1") > v("6.9.0"));
        assert!(v("1.2") == v("1.2.0"));
        assert_eq!(v("26.12.0").mayor(), 26);
    }

    #[test]
    fn la_revision_de_apk_se_ignora() {
        assert_eq!(v("3.5.1-r0"), v("3.5.1"));
        assert_eq!(v("3.5.1-r12"), v("3.5.1"));
    }

    #[test]
    fn prerelease_es_menor_que_la_final() {
        assert!(v("1.0.0-rc.1") < v("1.0.0"));
        assert!(v("1.0.0-rc1") > v("1.0.0-beta2"));
        assert!(v("1.0.0-beta") > v("1.0.0-alpha3"));
    }

    #[test]
    fn lo_irreconocible_no_se_adivina() {
        assert!(parsear("").is_none());
        assert!(parsear("abc").is_none());
        assert!(parsear("1.2.x").is_none());
        assert!(parsear("1.0.0-foo").is_none());
    }
}
