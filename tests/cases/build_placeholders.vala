namespace Config {
    public const string VERSION = @VERSION@;
    public const string DATADIR=@DATADIR@;
}

void main() {
    var v = @VERSION@;
    print("%s\n", @PACKAGE_NAME@ + " " + @VERSION@);
    if (@DEBUG_ENABLED@) {
        stdout.printf(@GETTEXT_PACKAGE@);
    }
}
