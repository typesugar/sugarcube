interface Nested<F> {
    wrap: <A>(a: A) => $<F, Array<A>>;
}
