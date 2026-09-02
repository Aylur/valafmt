public enum Color {
    RED,
    GREEN = 5,
    BLUE;

    public string to_hex() {
        switch (this) {
        case RED:
            return "#f00";
        default:
            return "#000";
        }
    }
}

public errordomain IOError {
    NOT_FOUND,
    PERMISSION_DENIED
}

private enum Small { A, B, C }
