void demo() {
    button.clicked.connect(() => {
        counter++;
        update();
    });
    list.foreach((item) => print("%s\n", item));
    var f = (a, b) => a + b;
    handler = x => {
        process(x);
    };
    timeout_add(500, () => {
        if (done) {
            return false;
        }
        return true;
    });
    items.sort((a, b) => {
        return strcmp(a.name, b.name);
    });
}
