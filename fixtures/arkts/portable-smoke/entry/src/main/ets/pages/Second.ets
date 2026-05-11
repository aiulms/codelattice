@Component
struct Second {
  @State count: number = 0;

  build() {
    Row() {
      Column() {
        Text(`Count: ${this.count}`)
          .fontSize(20)
        Button("Increment")
          .onClick(() => {
            this.count++;
          })
      }
    }
  }
}
