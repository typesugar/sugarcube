type Lift<F<_>> = <A, B>(f: (a: A) => B) => (fa: F<A>) => F<B>;
