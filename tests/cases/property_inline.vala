public class Inline : Object {
    internal List<weak Notification> notifications {
        get { return notification_list; }
    }
    internal string broken_source {
        get { return name;
        }
    }
    public int both { get { return a; } set { a = value; } }
    public int mixed {
        get { return a; }
        set {
            a = value;
            notify();
        }
    }
    public int expanded {
        get {
            return a;
        }
    }
}
