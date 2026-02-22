const add = (a: number, b: number): number => a + b;
const inc = (x: number) => x + 1;
const compose =
  <A, B, C>(f: (a: A) => B, g: (b: B) => C) =>
  (a: A): C =>
    g(f(a));
