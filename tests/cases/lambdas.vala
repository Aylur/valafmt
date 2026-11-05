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

void hugged_args() {
    noc_signal_id = bus.signal_subscribe(null, "org.freedesktop.DBus", "NameOwnerChanged", null, null, DBusSignalFlags.NONE, (connection, sender_name, path, interface_name, signal_name, parameters) => {
        string name = null;
        parameters.get("(sss)", &name, &old_owner, &new_owner);
    });
    noc_signal_id = bus.signal_subscribe(null, "org.freedesktop.DBus", "NameOwnerChanged", null, null, DBusSignalFlags.NONE, (connection, sender_name, path, interface_name, signal_name, parameters, hello, there, another, verylong, one) => {
        string name = null;
    });
    Bus.own_name(BusType.SESSION, application_id, BusNameOwnerFlags.NONE, (conn) => {
        try {
            this.conn = conn;
            conn.register_object("/io/Astal/Application", this);
        } catch (Error err) {
            critical(err.message);
        }
    }, () => {}, () => {});
    button.clicked.connect((a, b) => {
        print("hi");
    });
    Idle.add(() => long_function_name_here(argument_one, argument_two, argument_three, arg_four));
    foo(a, (x) => x + 1);
    Idle.add(() => long_function_name_here(argument_one, argument_two, argument_three, arg_four, a5));
    Timeout.add(100, (a, b) => {
        return false;
    });
    var id = group.items_changed.connect((pos, rem, add, asd) => handle_items_changed(group, pos, rem, add, asd));
    var id = group.items_changed.connect((pos, rem, add, asd, hello, there, whatpup, hasd, a123, aluwbdabdakwhb) => handle_items_changed(group, pos, rem, add, asd));
    var id = group.items_changed.connect((pos, rem, add, asd, hello, there, whatpup, hasd, a123, aluwbdabdakwhb, more, params) => handle_items_changed(group, pos, rem, add, asd));
}
