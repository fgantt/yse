# NNUE UI Integration Guide

## How NNUE is Loaded

**NNUE is automatically enabled by default** - no UI configuration needed!

When the engine starts:
1. It automatically looks for `nnue_weights_trained.json` in the **current working directory**
2. If found, NNUE is automatically enabled
3. If not found, the engine uses traditional evaluation (no error)

## Verifying NNUE Status in UI

### 1. Check `isready` Response

When your UI sends the `isready` command, the engine will respond with:

```
info string NNUE evaluation enabled (using trained weights)
readyok
```

This confirms NNUE is active. If you don't see this message, NNUE weights weren't found.

### 2. Check Search Output

During search, you'll see:
- First depth: `info depth 1 ... [NNUE]` (indicator on first depth line)
- Search start: `info string Using NNUE evaluation` (once per search)

### 3. No Configuration Needed

**You don't need to send any configuration commands** - NNUE is automatically loaded if the weights file exists.

## File Location

The engine looks for `nnue_weights_trained.json` in the **current working directory** where the engine process runs.

**Important**: The "current working directory" is where your UI launches the engine process from, not necessarily where the executable is located.

### Solutions if Weights Not Found

1. **Place weights file in the working directory**: Copy `nnue_weights_trained.json` to where your UI runs the engine

2. **Launch engine from the correct directory**: Make sure your UI's working directory contains the weights file

3. **Use absolute path** (requires code modification): Modify `src/lib.rs` `load_trained_nnue_weights()` to use an absolute path

4. **Check engine output**: Look for the `info string NNUE evaluation enabled` message in the `isready` response

## Testing in Your UI

1. **Start engine**: Your UI should start the engine process
2. **Send `usi` command**: Engine responds with available options
3. **Send `isready` command**: **Look for** `info string NNUE evaluation enabled (using trained weights)`
4. **If you see the message**: NNUE is active! ✅
5. **If you don't see it**: The weights file isn't in the working directory

## Expected USI Protocol Flow

```
UI → Engine: usi
Engine → UI: id name Yggdrasil
Engine → UI: id author fgantt (Gemini & Cursor)
Engine → UI: option name USI_Hash type spin default 16 min 1 max 1024
Engine → UI: ... (other options)

UI → Engine: isready
Engine → UI: info string NNUE evaluation enabled (using trained weights)  ← LOOK FOR THIS
Engine → UI: readyok

UI → Engine: position startpos
UI → Engine: go

Engine → UI: info string Using NNUE evaluation  ← During search
Engine → UI: info depth 1 seldepth 4 multipv 1 score cp -588 time 110 nodes 17 nps 154 pv 4c4d [NNUE]  ← [NNUE] indicator
Engine → UI: ... (more depth info)
Engine → UI: bestmove 4c4d
```

## Troubleshooting

### Problem: Not seeing NNUE status message

**Check:**
1. Does `nnue_weights_trained.json` exist in the working directory?
2. Is the file readable?
3. Is the engine process running from the correct directory?

**Solution:**
- Copy the weights file to the directory where your UI launches the engine
- Or modify the code to use an absolute path (see `src/lib.rs` line 209)

### Problem: Engine works but no NNUE message

**Check:**
- The message only appears if weights are successfully loaded
- If weights file doesn't exist, engine silently falls back to traditional eval
- This is normal behavior - no error is reported

**Solution:**
- Ensure `nnue_weights_trained.json` is in the working directory
- Check file permissions

## Summary

- ✅ **No configuration needed** - NNUE auto-loads if weights file exists
- ✅ **Check `isready` response** - Look for `info string NNUE evaluation enabled`
- ✅ **File location matters** - Weights file must be in the working directory
- ✅ **Visible indicators** - `[NNUE]` on first depth, "Using NNUE" message during search

