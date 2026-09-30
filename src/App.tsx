import { useLocation } from "react-router-dom";
import type { ComponentType } from "react";
import Layout from "./components/Layout";
import Home from "./modules/home";
import Clusters from "./modules/clusters";
// Cuando migren cada herramienta, se descomenta su import y su entrada en MODULES.
// import Cicd from "./modules/cicd";
// import Bitbucket from "./modules/bitbucket";

// Todos los módulos quedan montados SIEMPRE — nunca se desmontan al navegar
// entre pestañas del sidebar. Antes, <Routes> montaba/desmontaba el módulo
// activo en cada cambio de ruta, así que volver a "Clusters" perdía todo
// (cuentas descubiertas, resultados, historial de la consola). Ahora solo
// se alterna cuál es visible con CSS — el resto sigue vivo de fondo.
const MODULES: { path: string; Component: ComponentType }[] = [
  { path: "/", Component: Home },
  { path: "/clusters", Component: Clusters },
  // { path: "/cicd", Component: Cicd },
  // { path: "/bitbucket", Component: Bitbucket },
];

export default function App() {
  const location = useLocation();

  return (
    <Layout>
      {MODULES.map(({ path, Component }) => (
        <div key={path} style={{ display: location.pathname === path ? "block" : "none", height: "100%" }}>
          <Component />
        </div>
      ))}
    </Layout>
  );
}
