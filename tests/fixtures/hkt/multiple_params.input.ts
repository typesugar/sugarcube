interface BiFunctor<F<_>, G<_>> {
  bimap: <A, B, C, D>(fa: F<A>, gb: G<B>, f: (a: A) => C, g: (b: B) => D) => F<C>;
}
