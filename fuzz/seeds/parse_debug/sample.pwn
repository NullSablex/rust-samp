#include <a_samp>

new gCounter;

Helper(value)
{
    new local = value * 2;
    return local + gCounter;
}

main()
{
    gCounter = 1;
    printf("%d", Helper(21));
}
