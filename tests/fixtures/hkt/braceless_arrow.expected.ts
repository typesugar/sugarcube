type Lift<F> = <A, B>(f: (a: A) => B) => (fa: $<F, A>) => $<F, B>;
