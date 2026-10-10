import { useEffect, useState } from "react";

// Typewriter effect — types the string one character at a time, then idles.
export function useTypewriter(text: string, opts?: { speed?: number; delay?: number; loop?: boolean }) {
  const speed = opts?.speed ?? 90;
  const delay = opts?.delay ?? 0;
  const loop = opts?.loop ?? false;
  const [out, setOut] = useState("");
  const [done, setDone] = useState(false);

  useEffect(() => {
    setOut("");
    setDone(false);
    let i = 0;
    let timer: number | undefined;
    const start = window.setTimeout(function tick() {
      i += 1;
      setOut(text.slice(0, i));
      if (i < text.length) {
        timer = window.setTimeout(tick, speed);
      } else {
        setDone(true);
        if (loop) {
          timer = window.setTimeout(() => {
            i = 0;
            setOut("");
            setDone(false);
            tick();
          }, 2200);
        }
      }
    }, delay);
    return () => {
      window.clearTimeout(start);
      if (timer) window.clearTimeout(timer);
    };
  }, [text, speed, delay, loop]);

  return { text: out, done };
}
