interface IterableOnce<F> {
    toArray: <A>(fa: $<F, A>) => A[];
}
const list = __binop__(1, "::", __binop__(2, "::", []));
