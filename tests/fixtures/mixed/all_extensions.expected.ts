interface Functor<F> {
    map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
const result = __binop__(__binop__(data, "|>", parse), "|>", validate);
const list = __binop__(1, "::", __binop__(2, "::", []));
