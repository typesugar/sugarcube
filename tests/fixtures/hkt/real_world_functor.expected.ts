export interface Functor<F> {
    readonly map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
