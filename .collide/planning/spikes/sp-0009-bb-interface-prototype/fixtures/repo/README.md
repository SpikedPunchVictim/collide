# Larder

A small command-line tool that keeps track of what is in a kitchen: what, how much, where it is kept and when it
expires. It reminds you before things go off and prints a few reports. All names here are invented.

    larder add "2 cups flour" --shelf pantry --expires 2026-12-01
    larder list
    larder report low-stock
    larder report csv > stock.csv
