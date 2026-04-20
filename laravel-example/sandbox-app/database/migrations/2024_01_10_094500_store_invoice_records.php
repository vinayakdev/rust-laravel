<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

class CreateInvoicesTable extends Migration
{
    public function up(): void
    {
        Schema::create('invoices', function (Blueprint $table) {
            $table->id();
            $table->uuid('public_id')->unique();
            $table->foreignId('subscription_id')->references('id')->on('billing_subscriptions');
            $table->string('provider_invoice_id')->nullable();
            $table->decimal('subtotal_amount')->default(0);
            $table->decimal('tax_amount')->default(0);
            $table->decimal('total_amount')->default(0);
            $table->timestamp('paid_at')->nullable();
            $table->timestamps();
        });
    }
}
