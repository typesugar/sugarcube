interface Functor<F> {
    map: <A, B>(fa: $<F, A>, f: (a: A) => B) => $<F, B>;
}
