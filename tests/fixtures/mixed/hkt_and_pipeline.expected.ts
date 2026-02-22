interface Functor<F> {
    map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
const result = __binop__(value, "|>", transform);
