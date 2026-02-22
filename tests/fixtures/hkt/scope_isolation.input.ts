interface Functor<F<_>> {
  map: <A, B>(fa: F<A>, f: (a: A) => B) => F<B>;
}

interface Other<G> {
  get: <A>(ga: G<A>) => A;
}
