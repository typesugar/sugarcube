interface Container<F<_>> {
  get: <A>(fa: F<A>) => Array<A>;
}
