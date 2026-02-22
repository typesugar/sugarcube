interface Container<T> {
    value: T;
    map<U>(f: (t: T) => U): Container<U>;
}
function identity<T>(x: T): T {
    return x;
}
const c: Container<number> = {
    value: 42,
    map: (f)=>({
            value: f(42),
            map: c.map
        })
};
