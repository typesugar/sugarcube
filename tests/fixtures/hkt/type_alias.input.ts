type Apply<F<_>> = <A, B>(fa: F<A>, fab: F<(a: A) => B>) => F<B>;
