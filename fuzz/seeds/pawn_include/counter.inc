/*
 * counter v1.1.0 — a stateful example plugin for SA-MP and open.mp.
 *
 * GENERATED from counter.inc.in — do not edit counter.inc by hand.
 *
 * The prose, the sections and the callback documentation live in the template;
 * every `native` line comes from the Rust signature, so it cannot drift.
 */

#if defined _counter_included
    #endinput
#endif
#define _counter_included

#define COUNTER_VERSION "1.1.0"


// ---------------------------------------------------------------------------
// Callbacks
// ---------------------------------------------------------------------------

// Called on the main thread once a Counter_WorkAsync job has finished.
forward OnCounterWorkDone(delay);

// Declares a callback in one line — `Counter::OnCounterWorkDone(delay) { ... }`
// is the usual forward/public pair. Hand-written Pawn like this stays in the
// template; only the declarations are generated.
#define Counter::%0(%1) forward %0(%1); public %0(%1)


// ---------------------------------------------------------------------------
// Reading and changing the counter
// ---------------------------------------------------------------------------

// Documentation written once, in the Rust doc comment, rendered here in Pawn's
// own format — the one the open.mp includes use and `pawncc -r` reads.
/**
 * <summary>Adds one to the counter.</summary>
 * <returns>The new value, or -1 when already at the maximum.</returns>
 * <seealso name="Counter_Decrement" />
 */
native Counter_Increment();

/**
 * <summary>Decrements the counter. Returns the new value, or -1 if already zero.</summary>
 */
native Counter_Decrement();

// Back to zero.
native Counter_Reset();

// Reads the current value into `out`.
native bool:Counter_Get(&out);

// The open.mp-styled spelling of the same native, for a script that prefers it.
native bool:Counter_Read(&out) = Counter_Get;


// ---------------------------------------------------------------------------
// Everything else
// ---------------------------------------------------------------------------

native bool:Counter_SetMax(max = 100);
native bool:Counter_IsAtMax();
native bool:Counter_WorkAsync(delay_ms);
native Counter_NotifyScore(playerid);
native Counter_ListNatives(limit = 10);
