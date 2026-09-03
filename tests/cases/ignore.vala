public class Ignored : Object {
    // valafmt-ignore
    private int matrix   =   weird  (   1,2  ,3 );

    private int normal = weird(1, 2, 3);

    public void body() {
        // valafmt-ignore
        int    aligned_a =  1;
        int normal_stmt = 2;

        // valafmt-ignore-start
        int  x  =  1;
        int  yy =  2;
        int  zzz=  3;
        // valafmt-ignore-end
        call ( 1,2 );
    }

    // valafmt-ignore-start
    void   keep_me   ()   {
        exactly ( );
    }
    void   also_me () {}
    // valafmt-ignore-end
    void reformat_me() { call ( ); }
}

void tail() {
    switch (x) {
    case 1:
        // valafmt-ignore
        weird   (  );
        break;
    }
    // valafmt-ignore
    int last   =   1;
}
