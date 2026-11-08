import type { SunsetClient } from "@sunset/client";
import { useCallback, useEffect, useRef, useState } from "react";

type Phase = "idle" | "starting" | "recording" | "transcribing" | "cancelling" | "stop_failed";

/** Recording is transient. Only a reviewed transcript can become a conversation turn. */
export function useVoiceInput(client: SunsetClient, context: string, onTranscript: (text: string) => void) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [elapsedMs, setElapsedMs] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const phaseRef = useRef<Phase>("idle");
  const generation = useRef(0);
  const mounted = useRef(true);
  const current = useRef({ context, onTranscript });
  current.current = { context, onTranscript };
  const pendingStart = useRef<Promise<void> | null>(null);

  const changePhase = useCallback((next: Phase) => {
    phaseRef.current = next;
    if (mounted.current) setPhase(next);
  }, []);

  const cancel = useCallback(async () => {
    if (phaseRef.current === "idle" || phaseRef.current === "cancelling") return;
    ++generation.current;
    changePhase("cancelling");
    // If permission/device setup is still pending, stop it after start acknowledges. A cancel
    // sent before start completes could otherwise leave a late-starting microphone running.
    await pendingStart.current?.catch(() => {});
    try {
      await client.cancelRecording();
      changePhase("idle");
    } catch (err) {
      if (mounted.current) setError(`Could not confirm recording stopped: ${String(err)}`);
      changePhase("stop_failed");
    }
  }, [client, changePhase]);

  const finish = useCallback(async () => {
    if (phaseRef.current !== "recording") return;
    const request = generation.current;
    const capturedContext = current.current.context;
    changePhase("transcribing");
    try {
      const text = await client.stopRecording();
      if (mounted.current && request === generation.current && capturedContext === current.current.context) {
        if (text.trim()) current.current.onTranscript(text.trim());
        else setError("No speech was recognized. Try recording again.");
      }
    } catch (err) {
      if (mounted.current && request === generation.current) setError(String(err));
    } finally {
      if (request === generation.current) changePhase("idle");
    }
  }, [client, changePhase]);

  const start = useCallback(async () => {
    if (phaseRef.current !== "idle") return;
    const request = ++generation.current;
    const capturedContext = current.current.context;
    setError(null);
    setElapsedMs(0);
    changePhase("starting");
    try {
      const task = client.startRecording();
      pendingStart.current = task;
      await task;
      if (mounted.current && request === generation.current && capturedContext === current.current.context) {
        changePhase("recording");
      }
    } catch (err) {
      if (mounted.current && request === generation.current) {
        setError(String(err));
        changePhase("idle");
      }
    } finally {
      pendingStart.current = null;
    }
  }, [client, changePhase]);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      void cancel();
    };
  }, [cancel]);

  const previousContext = useRef(context);
  useEffect(() => {
    if (previousContext.current !== context) {
      previousContext.current = context;
      void cancel();
    }
  }, [context, cancel]);

  useEffect(() => {
    if (phase !== "recording") return;
    const request = generation.current;
    let disposed = false;
    let polling = false;
    const poll = async () => {
      if (polling) return;
      polling = true;
      try {
        const status = await client.recordingStatus();
        if (disposed || request !== generation.current || phaseRef.current !== "recording") return;
        setElapsedMs(status.elapsedMs);
        if (status.error) {
          setError(status.error);
          void cancel();
        } else if (status.autoStopped) {
          void finish();
        } else if (!status.recording) {
          setError("Recording stopped before it could be transcribed. Try again.");
          void cancel();
        }
      } catch (err) {
        if (!disposed && request === generation.current && phaseRef.current === "recording") {
          setError(String(err));
          void cancel();
        }
      } finally {
        polling = false;
      }
    };
    void poll();
    const interval = setInterval(() => void poll(), 500);
    return () => { disposed = true; clearInterval(interval); };
  }, [phase, client, cancel, finish]);

  return { phase, elapsedMs, error, busy: phase !== "idle", isBusy: () => phaseRef.current !== "idle", start, finish, cancel };
}
