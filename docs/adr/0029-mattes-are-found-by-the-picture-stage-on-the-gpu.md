# Mattes are found by the Picture stage, on the GPU, once per frame

A letterboxed or pillarboxed frame carries black bars. A Picture View that colours the picture (ARRI False Colour paints black purple, for one) would colour the bars and make a frame look crushed all round.

## Decision

- The Picture stage finds the **mattes** of every frame it shows: the rows at the top and bottom and the columns at the sides that are exactly black (red, green and blue all 0.0) from the edge inwards. Nothing in the picture is a matte unless a black row or column runs from the frame's edge to it; a black band inside the picture is coloured by the rule as before, and an all-black frame has no mattes.
- It is two small compute steps recorded into the encoder that uploads the frame (`mark`: flag each row and column holding a pixel that is not black; `find`: walk in from each edge). The four edges stay in a GPU buffer the picture's fragment shader reads; the picture's texels outside them are drawn black before the colour rule runs. Nothing is read back, so nothing waits for the GPU (ADR 0003).
- The legend strip and the "fill the Panel" message of Stops False Colour are not matted.
- Every Picture View goes through the same stage, so every one gets it. Views whose rule leaves black black (the clean picture, black and white, the heatmap and Skin Tone) look the same.

## Considered and rejected

- **Reading the bars back to the CPU:** a wait for the GPU, which ADR 0003 forbids.
- **Each view testing its own pixels:** nine copies of the same rule (ADR 0028).
- **Counting any near-black as a matte:** crushed shadows inside a letterboxed picture would turn black, hiding what the Panel exists to show. Exactly black only.

## Consequences

- One extra GPU job per frame, a few microseconds on the GPU; the Feed thread does not wait for it.
- A frame whose bars carry noise or a tiny value (not exactly black) is not matted: the bars are coloured. That is honest to the pixels.
