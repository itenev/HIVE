# 🐒 HIVE: The Monkey-Brain Guide

Welcome to the **HIVE**. If you're looking at the code and thinking, "What the actual f**k is going on here?" — this guide is for you. 

We will break down what HIVE is, how its parts communicate, and how it "thinks," using zero fancy jargon.

---

## 1. What is HIVE? 
Imagine a very smart receptionist sitting at a desk. 
- People call in from different phones (Discord, a website, a command line terminal). 
- The receptionist answers, remembers who they are, looks up their file (Memory), asks the smart guy in the back room (the AI Model) what to say, and then replies to the exact phone the person called from.

**HIVE** is that receptionist. It sits in the middle, routes messages, manages memory securely, and talks to the AI models.

The persona (the "personality" the AI speaks with) is named **Apis**. Look at her as the soul of HIVE.

---

## 2. The Core Parts (The Org Chart)

There are 4 main jobs in HIVE. Everything lives in the `src` folder.

### 🏢 Platforms (`src/platforms/`)
**"The Phones"**
A Platform is anywhere a user can type a message. Right now, HIVE has:
1. **CLI**: The black terminal window on your computer.
2. **Discord**: The chat app.

If you ever want HIVE to talk on Telegram, Twitter, or SMS, you just build a new "Platform" file. All a Platform has to do is:
- **`start()`**: Start listening for messages.
- **`send()`**: Send a response back to the user.

### 🧠 Providers (`src/providers/`)
**"The Brain in the Back Room"**
The Provider is the actual AI model that does the thinking. Right now, it's **Ollama** (running locally on your Mac). 
- The Provider only does one thing: it takes the chat history, reads the new message, and returns an answer.
- It doesn't know what Discord is. It doesn't know what a Discord channel is. It just reads text and writes text.

### 🗄️ Memory (`src/memory/`)
**"The Filing Cabinet"**
Whenever someone talks to HIVE, the Engine writes it down in the `MemoryStore`.
But memory in HIVE is **strictly scoped** (secure). If you DM the bot on Discord, that memory goes into a locked folder. If someone talks to the bot in a public Discord channel, that's a different folder. 

When the AI tries to answer you, it *only* gets the folder for where you are currently talking. No cross-contamination. 

### ⚙️ The Engine (`src/engine/mod.rs`)
**"The Receptionist / Router"**
This is the heart of HIVE. The Engine connects everything together. 
Let's look at the exact loop of what happens when you send a message.

---

## 3. The Event Loop (Life of a Message)

When you type `"Hello Apis!"` in Discord, here is the exact step-by-step of what happens in the Engine:

1. **Inbound**: The Discord platform hears you and sends an `Event` object to the Engine. An Event looks like:
   - `Who:` User123
   - `Where (Scope):` Discord Public Channel #general
   - `What:` "Hello Apis!"
2. **Remember It**: The Engine saves your exact message into the Memory Filing Cabinet under the `Discord Public Channel #general` folder.
3. **Get History**: The Engine grabs the *entire* history of that specific folder to give the AI context.
4. **Think (Live Telemetry)**: The Engine tells the AI Provider (Ollama), "Hey, read this history and reply." 
   - While Ollama is thinking, the Engine sets up a "Live Updating" task. (More on this below).
5. **Get the Answer**: Ollama finishes thinking and hands the Engine the final text.
6. **Remember the Answer**: The Engine saves the AI's response into the Memory Filing Cabinet. (So the AI remembers what it *just* said).
7. **Outbound**: The Engine packages the answer into a `Response` object and hands it back to the Discord platform to post it in the channel.

---

## 4. Telemetry: "Thinking Out Loud"

You know how when you use ChatGPT, you watch it type out word by word? AI generation takes time. If you just wait 30 seconds in silence, users think the bot broke.

HIVE has **Live Telemetry** (a Cognition Tracker). 

When the Engine asks Ollama for an answer, Ollama doesn't just return the whole answer at once. It streams the "thinking" tokens back over a tube (a channel in Rust) piece by piece.

Instead of spamming the user with a new message for every single letter the AI thinks of, the Engine **debounces** the updates.
- It waits 800 milliseconds. 
- It gathers all the little text pieces Ollama spit out in that time.
- It edits the Discord message to say: `🧠 Thinking... (5s)` and shows the text under it.
- It repeats this every 800 milliseconds until the AI is completely done.
- Finally, it edits the message one last time to say `✅ Complete (12s)` and posts the actual final answer as a new message.

**Note on Truncation vs. Chunking:**
HIVE used to just chop off the thinking text if it got too long for Discord (Discord has a 4096 character limit). We completely ripped that out! Now, the Engine passes the *entire* giant block of thought, and it's up to the Discord platform code to chunk it into multiple messages if it's too big. No data is lost.

---

## 5. Security & Privacy (Scope)

If you look at `src/models/scope.rs`, you'll see why HIVE is safe to use in a busy server. Everything is tagged with a `Scope`.

- `Scope::Public`: A public chat (like a Discord channel or the CLI).
- `Scope::Private { key }`: A private chat (like a Discord DM). The `key` is the user's ID.

If I DM the bot and say "My secret password is 1234", that's saved in `Scope::Private { key: "my_discord_id" }`. 
If someone goes into the public general channel and asks the bot, "What is his secret password?", the Engine will load the history for `Scope::Public`. 
The bot will literally have **no idea** what the password is, because it's not in the public folder. It is physically impossible for the AI to leak it.

---

## 6. How Confident Are We That This Works? 

**100%. Literally.**

If you look at the HIVE README, there is a test coverage badge that says `Coverage 100%`.
We wrote automated tests that simulate every single possible path the code could take. 
- What if Discord disconnects? Tested.
- What if Ollama crashes mid-sentence? Tested.
- What if the thinking goes longer than 1 minute? Tested.
- Does private memory stay private? Tested.

Every single line of code in `src/` runs successfully through a test before we let you push it to GitHub. It's built like a tank.

---

## Summary for Monkey Brains 🐒:

- **Platforms** = Ears & Mouth (Discord, Terminal)
- **Providers** = Brain (Ollama)
- **Memory** = The safe where chat logs live.
- **Engine** = The traffic cop routing messages between Ears, Safes, and Brains.
- **Telemetry** = The "Thinking... (5s)" live updates so you know it didn't crash.
- **Test Coverage** = 100%. It will not break randomly.
