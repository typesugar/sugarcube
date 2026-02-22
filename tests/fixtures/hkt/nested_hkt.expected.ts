interface Nested<F, G> {
    fg: <A>(fa: $<F, G<A>>) => $<G, F<A>>;
}

