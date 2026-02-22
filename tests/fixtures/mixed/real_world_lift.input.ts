export interface Functor<F<_>> {
  readonly map: <A, B>(fa: F<A>, f: (a: A) => B) => F<B>;
}

const pipeline = data |> parse |> validate |> transform;
const list = 1 :: 2 :: 3 :: 4 :: [];
