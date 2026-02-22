function log(target: any, key: string) {}
class MyClass {
    @log
    greet() {
        return "hello";
    }
}
