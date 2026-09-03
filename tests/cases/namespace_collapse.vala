namespace NS {
    class Klass {
        public int field;
    }
}

namespace Outer {
    namespace Inner {
        public class Deep : Object {
            construct {
                init();
            }
        }
    }
}

// doc comment stays with the class
namespace Documented {
    /**
     * The class itself.
     */
    public class WithDocs : Object {
    }
}

namespace Multi {
    class A {
    }

    class B {
    }
}

namespace WithUsing {
    class OnlyButNotAlone {
    }
    private int helper;
}

namespace Dotted.Name {
    class C {
    }
}
