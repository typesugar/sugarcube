interface MapLike<F<_>, K> {
  get: <V>(fa: F<V>, key: K) => V;
}
