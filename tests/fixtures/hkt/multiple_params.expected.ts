interface BiFunctor<F, G> {
    bimap: <A, B, C, D>(fa: $<F, A>, gb: $<G, B>, f: (a: A) => C, g: (b: B) => D) => $<F, C>;
}
