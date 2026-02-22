interface Container<F> {
    get: <A>(fa: $<F, A>) => Array<A>;
}
