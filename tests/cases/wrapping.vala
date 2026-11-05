void wrapping() {
    var result = some_object.some_method_with_long_name(first_argument, second_argument, third_argument, fourth_argument);
    var chained = builder.add_first_component().add_second_component().add_third_component().add_fourth_component().build();
    var condition_result = first_long_condition_value && second_long_condition_value && third_long_condition_value && fourth_value;
    var ternary_result = some_long_condition_expression ? quite_long_result_when_true : quite_long_result_when_false_case;
    string concatenated = "first long segment of text " + "second long segment of text " + "third long segment of text";
    opt(new Flag("wait", 'w'));
    opt(new Flag("wait", 'w', "Wait for the notification to be closed before exiting, really long"));
    add(new Gtk.Label("x") { halign = Gtk.Align.START, valign = Gtk.Align.CENTER, wrap = true, xalign = 0 });
    var x = new Foo(new Bar("some long argument here", "another long argument here", "and a third one"));
    opt(wait = new Flag("wait", 'w', "Wait for the notification to be closed before exiting, longer"));
    foo(a, new Flag("wait", 'w', "Wait for the notification to be closed before exiting, really long"));
}

public class LongDecl : Object {
    public void method_with_many_parameters(string first_param, int second_param, double third_param, bool fourth_param, Object fifth_param) {
    }
}
