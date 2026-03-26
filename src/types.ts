export type ReindexStatus = {
  indexed: number;
  removed: number;
  failed: number;
};

export type SearchResult = {
  title: string;
  path: string;
  snippet: string;
  fileType: string;
  modifiedTs: number;
};
