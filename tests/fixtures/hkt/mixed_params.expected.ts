interface MapLike<F, K> {
    get: <V>(fa: $<F, V>, key: K) => V;
}
