import { useEffect, useState } from "react";

const STORAGE_KEY = "devops-hub-font-scale";
const STEPS = [87.5, 100, 112.5, 125, 137.5];
const DEFAULT_STEP_INDEX = 1; // 100%

// Cambia el font-size del <html> — como el resto de la app usa unidades
// "rem" (Tailwind), esto escala TODO el texto proporcionalmente sin tocar
// ningún componente individual.
export function useFontScale() {
  const [stepIndex, setStepIndex] = useState(() => {
    const saved = localStorage.getItem(STORAGE_KEY);
    const parsed = saved ? Number(saved) : DEFAULT_STEP_INDEX;
    return Number.isFinite(parsed) && parsed >= 0 && parsed < STEPS.length ? parsed : DEFAULT_STEP_INDEX;
  });

  useEffect(() => {
    document.documentElement.style.fontSize = `${STEPS[stepIndex]}%`;
    localStorage.setItem(STORAGE_KEY, String(stepIndex));
  }, [stepIndex]);

  return {
    percent: STEPS[stepIndex],
    canIncrease: stepIndex < STEPS.length - 1,
    canDecrease: stepIndex > 0,
    increase: () => setStepIndex((i) => Math.min(i + 1, STEPS.length - 1)),
    decrease: () => setStepIndex((i) => Math.max(i - 1, 0)),
    reset: () => setStepIndex(DEFAULT_STEP_INDEX),
  };
}
