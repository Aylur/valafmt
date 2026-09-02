public class Portable : Object {
#if WINDOWS
    private const string SEP = "\\";
#else
    private const string SEP = "/";
#endif

    public void run() {
#if DEBUG
        print("debug\n");
#endif
        work();
    }
}
