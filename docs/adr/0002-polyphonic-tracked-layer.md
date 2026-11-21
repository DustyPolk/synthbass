# Polyphonic tracked layer via a semitone filterbank

The tracked layer plays up to four voices at once from a single ordinary pickup, by splitting the dry signal into semitone-spaced bands and giving each active band a voice, rather than detecting a single monophonic pitch. We accept that octaves and fifths may fuse into one voice and that this costs more CPU and some latency, because retrofitting polyphony onto a monophonic tracker later would mean rewriting it. This is an experiment: if tracking proves unclean, falling back to monophonic tracking is the escape hatch.
