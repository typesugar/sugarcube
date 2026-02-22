interface Functor<F<_>> {
  map: <A, B>(fa: F<A>, f: (a: A) => B) => F<B>;
}

const result = data |> parse |> validate;
const list = 1 :: 2 :: [];
