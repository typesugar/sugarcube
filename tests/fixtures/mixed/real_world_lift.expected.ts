export interface Functor<F> {
    readonly map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
const pipeline = __binop__(__binop__(__binop__(data, "|>", parse), "|>", validate), "|>", transform);
const list = __binop__(1, "::", __binop__(2, "::", __binop__(3, "::", __binop__(4, "::", []))));
