// Package fundb provides a Go SDK for FunDB, the AI-native cognitive database.
//
// Quick start:
//
//	conn, err := fundb.Connect(context.Background(), fundb.Config{
//	    Host: "localhost", Port: 5433,
//	    Database: "fundb", User: "fundb",
//	})
//	if err != nil { log.Fatal(err) }
//	defer conn.Close(context.Background())
//
//	result, err := conn.Execute(ctx, "SELECT * FROM documents LIMIT 10")
//
// Agent memory:
//
//	mem := conn.Memory("my-agent")
//	id, err := mem.Remember(ctx, "User prefers Go", fundb.RememberOptions{Importance: 0.8})
//	records, err := mem.Recall(ctx, "language preferences", fundb.RecallWeights{Semantic: 0.5, Recency: 0.3, Importance: 0.2}, 10)
package fundb
