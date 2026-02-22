type Foo<F<_>> = (fa: F<A>) => F<B>;
type Bar<G> = G<A>;
