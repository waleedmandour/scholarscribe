// ScholarScribe Word Assist — hand-written Office.js ambient declarations.
//
// DELIBERATELY MINIMAL: only the surface the client actually calls, with
// shapes as spike-verified (word-addin/spike/OBSERVATIONS.md experiments
// (a)-(m)) and as documented on learn.microsoft.com (WordApi 1.6/1.7/1.8
// pages, accessed 2026-10-23). This file exists so the client can be
// TypeScript-strict WITHOUT adding @types/office-js as a dependency (the
// v0.1 rule: no new dependency without a proposal). Anything beyond this
// surface is a deliberate PR with a spike reference.
//
// The declarations are intentionally wider than `any` nowhere: every
// member used by the client is named; unused Office surface is omitted.

declare namespace Office {
  interface AsyncResult<T> {
    status: AsyncResultStatus;
    error: { message: string; name: string };
    value: T;
  }

  enum AsyncResultStatus {
    Succeeded = "succeeded",
    Failed = "failed",
  }

  interface Requirements {
    isSetSupported(name: string, minVersion: number): boolean;
  }

  interface Context {
    readonly requirements: Requirements;
    readonly document: Document;
  }

  /** Global Office namespace surface used by the client. */
  const context: Context;

  /**
   * Event registration via the classic (1.1) model. The add-in uses it for
   * DocumentParagraphChanged; annotation events use the Word.js object
   * model (Word.Document.onAnnotation*).
   */
  interface Document {
    addHandlerAsync(
      eventType: string,
      handler: (result: AsyncResult<unknown>) => void,
      callback?: (asyncResult: AsyncResult<null>) => void,
    ): void;
  }

  interface HostReadyInfo {
    host: string;
    platform: string;
  }

  function onReady(
    cb: (info: HostReadyInfo) => void,
  ): void;

  namespace EventType {
    const DocumentParagraphChanged: string;
  }
}

declare namespace Word {
  /** Insert locations used by the client. */
  type InsertLocation = "Replace" | "End" | "Start" | "After" | "Before";

  /** Word.Critique payload for paragraph.insertAnnotations (spike (c)). */
  interface Critique {
    start: number;
    length: number;
    colorScheme?: string;
    popupOptions?: { suggestions: string[] };
  }

  interface Paragraph {
    readonly text: string;
    insertText(text: string, insertLocation: InsertLocation): void;
    /** WordApi 1.7. Errors are caught by the caller and degrade to list
     *  mode (never hard-fail the pane). */
    insertAnnotations(critiques: Critique[]): void;
  }

  interface ParagraphCollection {
    load(propertyNames?: string): void;
    readonly items: Paragraph[];
  }

  interface AnnotationPopupActionEventArgs {
    /** Annotation id as Word reports it (spike (j): correlation is by
     *  suffix match against our issue ids — mechanism still NOT VERIFIED
     *  on all platforms; QA matrix row). */
    id: string;
    /** The suggestion the user chose, or null when the popup was
     *  dismissed. */
    critiqueSuggestion: string | null;
  }

  interface RequestContext {
    readonly document: {
      readonly body: { readonly paragraphs: ParagraphCollection };
      onAnnotationPopupAction: {
        add(handler: (args: AnnotationPopupActionEventArgs) => void): void;
      };
    };
    sync(): Promise<void>;
  }

  function run<T>(batch: (ctx: RequestContext) => Promise<T>): Promise<T>;
}
