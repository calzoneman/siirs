# siirs

A collection of tools for interacting with ETS2/ATS save data for my own
personal use.  In particular, I was building the components I needed for
`report_achivements` to keep track of my progress on achievements that require
visiting cities or making deliveries to specific companies, which is not exposed
to you by the game.

To do that, I ended up building:

  * A binary sii save file decryptor and parser.
  * An SCS file extractor (but only for known hashes -- I didn't implement
    directory entry parsing).
  * A text sii parser, that is only good enough to parse the achievement
    definitions and `en_us` locale definition.
  * A decryptor for the XOR encryption format used for the locale files.
  * A translator to store the parsed binary sii save as a SQLite database.

This wasn't really built to be consumed by others as a library, but if you have
a use for it, let me know.

Many thanks to <https://github.com/TheLazyTomcat/SII_Decrypt> for the detailed
documentation on the binary save format and reference code for the decryption.
