void main() {
    var a = new MyClass() {
        field = "",
    };
    var b = new MyClass() {
        field = ""
    };
    var c = new MyClass() { field = "", };
    MyStruct s = { 1, 2, };
    MyStruct t = {
        1, 2
    };
    int[] arr = { 1, 2, 3, };
    var d = Foo() { x = 1, };
    var e = Foo() { x = 1 };
}
