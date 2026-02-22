interface Nested<F<_>, G<_>> {
  fg: <A>(fa: F<G<A>>) => G<F<A>>;
}
