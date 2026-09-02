using Gtk;
using GLib, Gee;

namespace Demo.Core {
    public class Widget : Object {
        private int count = 0;
        public const string NAME = "widget";

        public int add(int a, int b) {
            return a + b;
        }
    }
}

int main(string[] args) {
    return 0;
}
