interface IterableOnce<F<_>> {
  toArray: <A>(fa: F<A>) => A[];
}

const list = 1 :: 2 :: [];
