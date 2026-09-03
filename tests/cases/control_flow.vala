void flow(int x) {
    if (x > 0) {
        pos();
    } else if (x < 0) {
        neg();
    } else {
        zero();
    }

    if (x == 1) one();

    while (x > 0) {
        x--;
    }

    do {
        x++;
    } while (x < 10);

    for (int i = 0; i < 10; i++) {
        sum += i;
    }

    for (;;) {
        break;
    }

    foreach (var item in items) {
        process(item);
    }

    switch (x) {
    case 1:
        one();
        break;
    case 2:
    case 3:
        few();
        break;
    default:
        many();
        break;
    }
}

void case_blocks(string type) {
    switch (type) {
    case "int":
        {
            hints.set(name, new Variant.int32(int.parse(parts[2])));
            return;
        }
    case "double": {
        set_double();
        break;
    }
    case "scoped":
        {
            var tmp = 1;
        }
        break;
    }
}
