interface Nested<F<_>> {
  wrap: <A>(a: A) => F<Array<A>>;
}
