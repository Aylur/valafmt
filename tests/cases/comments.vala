// File header comment.

/**
 * Documented class.
 *
 * @since 1.0
 */
public class Doc : Object {
    // leading comment
    private int a; // trailing comment

    /* block comment */
    private int b;

    public void method() {
        // comment inside body

        call(); // after call
        /* before */ other();
    }

    private void empty_with_comment() {
        // only a comment inside
    }
}
